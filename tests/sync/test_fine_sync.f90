program test_fine_sync
  ! Generates fine sync reference data from WSJT-X ft8b.f90
  ! Compares refined frequency and time offsets for each candidate

  use iso_c_binding, only: c_float
  implicit none

  integer, parameter :: NMAX=15*12000
  real dd(NMAX)
  real*4 candidate_data(4,300)  ! freq, time, sync, baseline
  integer ncands
  real f1, xdt, xbase, xsnr
  integer nharderrors, nbadcrc, ipass, nQSOProgress, nfqso, nftx
  integer ndepth, nzhsym, napwid, iaptype, ncontest
  integer apsym(58), aph10(10), itone(79)
  character*37 msg37
  character*12 mycall12, hiscall12
  logical newdat, lapon, lapcqonly, lsubtract, nagain
  integer i, nargs
  character(len=256) :: wav_path, cand_path
  real dmin

  ! Get command line arguments
  nargs = command_argument_count()
  if (nargs .lt. 2) then
    print *, 'Usage: test_fine_sync <wav_file> <candidates.csv>'
    stop
  endif
  call get_command_argument(1, wav_path)
  call get_command_argument(2, cand_path)

  ! Load WAV file
  call read_wav(trim(wav_path), dd, NMAX)

  ! Load coarse sync candidates from CSV
  call load_candidates(trim(cand_path), candidate_data, ncands)

  print *, 'Testing fine sync on ', ncands, ' candidates'
  print *, 'freq_in,time_in,sync_in,freq_out,time_out,sync_out,nharderrors,nbadcrc'

  ! Initialize ft8b parameters (matching ft8_decode.f90 defaults)
  newdat = .true.
  nQSOProgress = 0
  nfqso = 1000
  nftx = 1000
  ndepth = 3  ! Maximum depth
  nzhsym = 50
  lapon = .false.
  lapcqonly = .false.
  napwid = 50
  lsubtract = .false.
  nagain = .false.
  ncontest = 0
  iaptype = 0
  mycall12 = '            '
  hiscall12 = '            '
  xbase = 0.0
  apsym = 0
  aph10 = 0

  ! Process each candidate
  do i = 1, ncands
    f1 = candidate_data(1, i)
    xdt = candidate_data(2, i)
    xbase = candidate_data(4, i)  ! baseline noise from coarse sync

    ! Call ft8b to refine this candidate
    ! NOTE: ft8b may decode the message (nbadcrc=0) or fail (nbadcrc=1)
    ! We're only interested in the refined f1 and xdt, not the decode
    dmin = 0.0
    call ft8b(dd, newdat, nQSOProgress, nfqso, nftx, ndepth, nzhsym, &
              lapon, lapcqonly, napwid, lsubtract, nagain, ncontest, &
              iaptype, mycall12, hiscall12, f1, xdt, xbase, apsym, &
              aph10, nharderrors, dmin, nbadcrc, ipass, msg37, xsnr, itone)

    ! Output: freq_in, time_in, sync_in, freq_out, time_out, sync_out
    ! NOTE: xsnr is the refined SNR estimate, which we can use as sync_out
    write(*, '(F8.1,",",F7.3,",",F7.3,",",F8.1,",",F7.3,",",F7.3,",",I3,",",I1)') &
      candidate_data(1, i), candidate_data(2, i), candidate_data(3, i), &
      f1, xdt, xsnr, nharderrors, nbadcrc
  enddo

end program test_fine_sync

subroutine read_wav(filename, dd, nmax)
  ! Read WAV file into dd array (simplified version)
  character(len=*) :: filename
  real dd(nmax)
  integer nmax, npts, i
  integer*2 iwave(nmax)

  ! Open WAV file (skip 44-byte header)
  open(10, file=filename, access='stream', status='old', form='unformatted')
  read(10, pos=45) iwave(1:nmax)  ! Read audio data
  close(10)

  ! Convert int16 to float32 WITHOUT normalization (matches WSJT-X)
  do i = 1, nmax
    dd(i) = real(iwave(i))
  enddo
end subroutine read_wav

subroutine load_candidates(filename, candidate_data, ncands)
  ! Load candidates from CSV: freq,time,sync,baseline
  character(len=*) :: filename
  real*4 candidate_data(4,300)
  integer ncands, ios
  real freq, time, sync, baseline

  open(11, file=filename, status='old')
  read(11, *)  ! Skip header line

  ncands = 0
  do
    read(11, *, iostat=ios) freq, time, sync, baseline
    if (ios /= 0) exit
    ncands = ncands + 1
    candidate_data(1, ncands) = freq
    candidate_data(2, ncands) = time
    candidate_data(3, ncands) = sync
    candidate_data(4, ncands) = baseline
  enddo
  close(11)
end subroutine load_candidates
