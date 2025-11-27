program test_sync2d
  ! Test program to generate reference sync2d data from WSJT-X
  ! This computes the 2D sync correlation matrix used for candidate selection

  include 'ft8_params.f90'

  integer, parameter :: NPTS=180000
  integer, parameter :: JZ=62  ! ±62 lags (MAX_LAG)
  real :: dd(NPTS)
  real :: s(NH1,NHSYM)
  real :: savg(NH1)
  real :: sync2d(NH1,-JZ:JZ)
  real :: x(NFFT1+2)
  complex :: cx(0:NH1)
  equivalence (x,cx)
  integer :: i, j, ia, ib, n, m, lag
  real :: fac, df, tstep
  integer :: nssy, nfos, jstrt
  real :: ta, tb, tc, t0a, t0b, t0c, t, t0
  real :: sync_abc, sync_bc
  integer :: icos7(0:6)
  integer :: key_bins(20)
  integer :: bin, k
  real :: freq
  data icos7/3,1,4,0,6,5,2/  ! Costas array pattern
  ! Original 10 bins + 10 new bins for better coverage (sorted by sync power)
  ! Bins: 823(237.8), 811(67.5), 813(60.4), 835(32.2), 690(31.8), 678(26.0), 703(18.9), 705(16.9),
  !       686(16.4), 189(15.8), 371(13.2), 231(13.0), 862(12.7), 137(11.0), 680(10.8), 477(1.6),
  !       478(1.6), 482(1.4), 347(edge), 458(edge)
  data key_bins/823, 811, 813, 835, 690, 678, 703, 705, 686, 189, &
                 371, 231, 862, 137, 680, 477, 478, 482, 347, 458/

  ! Read WAV file (210703_133430.wav)
  call read_wav_file('tests/test_data/210703_133430.wav', dd, NPTS)

  ! Compute spectra (matching sync8.f90)
  savg = 0.
  fac = 1.0/300.0

  do j=1,NHSYM
     ia = (j-1)*NSTEP + 1
     ib = ia + NSPS - 1
     x(1:NSPS) = fac*dd(ia:ib)
     x(NSPS+1:) = 0.
     call four2a(x,NFFT1,1,-1,0)              !r2c FFT
     do i=1,NH1
        s(i,j) = real(cx(i))**2 + aimag(cx(i))**2
     enddo
     savg = savg + s(1:NH1,j)
  enddo

  ! Compute sync2d (matching sync8.f90 lines 48-85)
  df = 12000.0 / NFFT1
  tstep = NSTEP / 12000.0
  nssy = NSPS / NSTEP   ! 4
  nfos = NFFT1 / NSPS   ! 2
  jstrt = 0.5 / tstep   ! CRITICAL: Integer division truncates 12.5 -> 12 (matches sync8.f90 line 50)

  print *, 'WSJT-X Sync2d Test Data'
  print *, 'NH1=', NH1, ' NHSYM=', NHSYM
  print *, 'nssy=', nssy, ' nfos=', nfos, ' jstrt=', jstrt
  print *, 'df=', df, ' tstep=', tstep
  print *, ''

  ! Compute sync2d for all bins and lags
  do i=1,NH1
     do lag=-JZ,JZ
        j = lag
        ta = 0.
        tb = 0.
        tc = 0.
        t0a = 0.
        t0b = 0.
        t0c = 0.

        do n=0,6
           m = j + jstrt + nssy*n

           ! Costas array 1 (symbols 0-6)
           if(m.ge.1 .and. m.le.NHSYM) then
              ta = ta + s(i+nfos*icos7(n),m)
              t0a = t0a + sum(s(i:i+nfos*6:nfos,m))
           endif

           ! Costas array 2 (symbols 36-42) - NO bounds check
           tb = tb + s(i+nfos*icos7(n),m+nssy*36)
           t0b = t0b + sum(s(i:i+nfos*6:nfos,m+nssy*36))

           ! Costas array 3 (symbols 72-78)
           if(m+nssy*72 .le. NHSYM) then
              tc = tc + s(i+nfos*icos7(n),m+nssy*72)
              t0c = t0c + sum(s(i:i+nfos*6:nfos,m+nssy*72))
           endif
        enddo

        ! Compute sync metric
        t = ta + tb + tc
        t0 = t0a + t0b + t0c
        t0 = (t0 - t) / 6.0
        if(t0 .gt. 0.0) then
           sync_abc = t / t0
        else
           sync_abc = 0.0
        endif

        ! Also try without first Costas
        t = tb + tc
        t0 = t0b + t0c
        t0 = (t0 - t) / 6.0
        if(t0 .gt. 0.0) then
           sync_bc = t / t0
        else
           sync_bc = 0.0
        endif

        sync2d(i,lag) = max(sync_abc, sync_bc)
     enddo
  enddo

  ! Output key bins where WSJT-X found candidates
  print *, 'Sync2d values at key WSJT-X candidate bins:'
  print *, '============================================='

  ! Bin 477 (1490.6 Hz) - WSJT-X found peak at lag=1
  print *, ''
  print *, 'Bin 477 (1490.6 Hz) - expect peak at lag=1:'
  do lag=-5,5
     print '(A,I4,A,E16.8)', '  lag=', lag, ' sync2d=', sync2d(477,lag)
  enddo

  ! Bin 478 (1493.8 Hz) - WSJT-X found peak at lag=2
  print *, ''
  print *, 'Bin 478 (1493.8 Hz) - expect peak at lag=2:'
  do lag=-5,5
     print '(A,I4,A,E16.8)', '  lag=', lag, ' sync2d=', sync2d(478,lag)
  enddo

  ! Bin 482 (1506.2 Hz) - WSJT-X found peak at lag=10
  print *, ''
  print *, 'Bin 482 (1506.2 Hz) - expect peak at lag=10:'
  do lag=5,15
     print '(A,I4,A,E16.8)', '  lag=', lag, ' sync2d=', sync2d(482,lag)
  enddo

  ! Bin 823 (2571.9 Hz) - WSJT-X found peak at lag=8
  print *, ''
  print *, 'Bin 823 (2571.9 Hz) - expect peak at lag=8:'
  do lag=3,13
     print '(A,I4,A,E16.8)', '  lag=', lag, ' sync2d=', sync2d(823,lag)
  enddo

  ! Bin 811 (2534.4 Hz) - WSJT-X found peak at lag=60
  print *, ''
  print *, 'Bin 811 (2534.4 Hz) - expect peak at lag=60:'
  do lag=55,62
     print '(A,I4,A,E16.8)', '  lag=', lag, ' sync2d=', sync2d(811,lag)
  enddo

  ! Write full sync2d for a few key bins to CSV
  print *, ''
  print *, 'Writing sync2d for key bins to tests/sync/sync2d_ref.csv...'
  open(20, file='tests/sync/sync2d_ref.csv', status='replace', action='write')

  ! Write header
  write(20, '(A)', advance='no') 'bin,freq'
  do lag=-JZ,JZ
     if(lag .lt. JZ) then
        write(20, '(A,I4,A)', advance='no') ',lag', lag, ''
     else
        write(20, '(A,I4)') ',lag', lag
     endif
  enddo

  ! Write data for key bins
  do k=1,20
     bin = key_bins(k)
     freq = bin * df
     write(20, '(I4,A,F8.2)', advance='no') bin, ',', freq
     do lag=-JZ,JZ
        if(lag .lt. JZ) then
           write(20, '(A,E16.8)', advance='no') ',', sync2d(bin,lag)
        else
           write(20, '(A,E16.8)') ',', sync2d(bin,lag)
        endif
     enddo
  enddo
  close(20)
  print *, 'Done writing sync2d_ref.csv'

end program test_sync2d

subroutine read_wav_file(filename, data, npts)
  character*(*) :: filename
  integer :: npts
  real :: data(npts)
  integer*2 :: iwave(npts)
  integer :: i

  open(10, file=filename, status='old', access='stream', form='unformatted')
  read(10, pos=45) iwave
  do i = 1, npts
    data(i) = real(iwave(i))
  enddo
  close(10)
end subroutine read_wav_file
