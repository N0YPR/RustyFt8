program test_spectra
  ! Test program to generate reference spectra data from WSJT-X
  ! This computes the power spectra used as input to sync8

  include 'ft8_params.f90'

  integer, parameter :: NPTS=180000
  real :: dd(NPTS)
  real :: s(NH1,NHSYM)
  real :: savg(NH1)
  real :: x(NFFT1+2)
  complex :: cx(0:NH1)
  equivalence (x,cx)
  integer :: i, j, ia, ib
  real :: fac

  ! Read WAV file
  call read_wav_file('tests/test_data/210703_133430.wav', dd, NPTS)

  ! Compute spectra (matching sync8.f90 lines 29-43)
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
     savg = savg + s(1:NH1,j)                 !Average spectrum
  enddo

  ! Output reference data for Rust tests
  print *, 'WSJT-X Spectra Test Data'
  print *, 'NH1=', NH1, ' NHSYM=', NHSYM, ' NSTEP=', NSTEP, ' NSPS=', NSPS, ' NFFT1=', NFFT1
  print *, ''

  ! Output specific bins that we'll check in Rust tests
  ! These bins are used in sync2d correlation (bin 477, 478, 482, 811, 823)
  print *, 'Spectra at key frequency bins (format: bin, time_step, power)'
  print *, '================================================================'

  ! Bin 477 (1490.6 Hz) at various time steps
  do j=1,20
     print '(A,I4,A,I4,A,E16.8)', 'bin=', 477, ' time=', j, ' power=', s(477,j)
  enddo

  print *, ''
  print *, 'Bin 478 at select times:'
  do j=1,20
     print '(A,I4,A,I4,A,E16.8)', 'bin=', 478, ' time=', j, ' power=', s(478,j)
  enddo

  print *, ''
  print *, 'Bin 482 at select times:'
  do j=1,20
     print '(A,I4,A,I4,A,E16.8)', 'bin=', 482, ' time=', j, ' power=', s(482,j)
  enddo

  print *, ''
  print *, 'Average spectrum at key bins:'
  print *, '============================='
  print '(A,I4,A,E16.8)', 'bin=', 477, ' avg=', savg(477)
  print '(A,I4,A,E16.8)', 'bin=', 478, ' avg=', savg(478)
  print '(A,I4,A,E16.8)', 'bin=', 482, ' avg=', savg(482)
  print '(A,I4,A,E16.8)', 'bin=', 811, ' avg=', savg(811)
  print '(A,I4,A,E16.8)', 'bin=', 823, ' avg=', savg(823)

  print *, ''
  print *, 'First 10 frequency bins at time step 13:'
  print *, '=========================================='
  do i=1,10
     print '(A,I4,A,E16.8)', 'bin=', i, ' power=', s(i,13)
  enddo

  print *, ''
  print *, 'Cross-check: accessing middle Costas time indices'
  print *, '=================================================='
  ! For lag=1 at bin 477: m values are 13,17,21,25,29,33,37 (n=0..6)
  ! Middle Costas at m+nssy*36 = 13+4*36 = 157, 17+144=161, etc.
  print *, 'Time indices for middle Costas (lag=1, bin=477):'
  print '(A,E16.8)', 'bin=477 time=157 power=', s(477,157)
  print '(A,E16.8)', 'bin=477 time=161 power=', s(477,161)
  print '(A,E16.8)', 'bin=477 time=165 power=', s(477,165)

  ! Write full spectra array to CSV file for comprehensive testing
  print *, ''
  print *, 'Writing full spectra to tests/sync/spectra.csv...'
  open(20, file='tests/sync/spectra.csv', status='replace', action='write')
  do i=1,NH1
     do j=1,NHSYM
        if (j < NHSYM) then
           write(20, '(E16.8,A)', advance='no') s(i,j), ','
        else
           write(20, '(E16.8)') s(i,j)
        endif
     enddo
  enddo
  close(20)
  print *, 'Done writing spectra.csv (', NH1, 'x', NHSYM, 'values)'

  ! Write average spectrum to CSV
  print *, 'Writing average spectrum to tests/sync/avg_spectrum.csv...'
  open(21, file='tests/sync/avg_spectrum.csv', status='replace', action='write')
  do i=1,NH1
     if (i < NH1) then
        write(21, '(E16.8,A)', advance='no') savg(i), ','
     else
        write(21, '(E16.8)') savg(i)
     endif
  enddo
  close(21)
  print *, 'Done writing avg_spectrum.csv (', NH1, 'values)'

end program test_spectra

subroutine read_wav_file(filename, data, npts)
  character*(*) :: filename
  integer :: npts
  real :: data(npts)
  integer*2 :: iwave(npts)
  integer :: i

  ! Open and read WAV file (simple 16-bit PCM reader)
  open(10, file=filename, status='old', access='stream', form='unformatted')

  ! Skip 44-byte WAV header
  read(10, pos=45) iwave

  ! Convert to real
  do i = 1, npts
    data(i) = real(iwave(i))
  enddo

  close(10)
end subroutine read_wav_file
